use crate::goals::goal::ExecuteGoal;
use crate::goals::{path_to_action, use_direction};
use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

pub struct KillEnemyGoal(pub Position);

impl ExecuteGoal for KillEnemyGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let enemy_pos = self.0;

        // If adjacent, attack
        if player_pos.is_adjacent(&enemy_pos) {
            return Some(use_direction(player_pos, enemy_pos));
        }

        // Move adjacent to enemy
        for adjacent in enemy_pos.neighbors() {
            if world.map.is_walkable(&adjacent, adjacent)
                && let Some(path) = world.find_path_for_player(player_index, player_pos, adjacent)
            {
                world.players[player_index].current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        None
    }
}
