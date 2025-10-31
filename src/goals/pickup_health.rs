use tracing::debug;

use crate::goals::goal::ExecuteGoal;
use crate::goals::path_to_action;
use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

pub struct PickupHealthGoal(pub Position);

impl ExecuteGoal for PickupHealthGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let health_pos = self.0;
        debug!("PickupHealth: going to destination {:?}", health_pos);
        world.players[player_index].current_destination = Some(health_pos);
        let path = world.find_path_for_player(player_index, player_pos, health_pos)?;
        debug!("PickupHealth: path length={}", path.len());
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
