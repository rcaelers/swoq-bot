use crate::goals::goal::ExecuteGoal;
use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

pub struct AvoidEnemyGoal(pub Position);

impl AvoidEnemyGoal {
    fn flee_direction(&self, world: &WorldState, player_index: usize) -> Option<DirectedAction> {
        let enemy_pos = self.0;
        // Move away from enemy - choose direction that maximizes distance
        // Only consider walkable positions
        let mut best_action = None;
        let player = &world.players[player_index];
        let player_pos = player.position;
        let mut best_distance = player_pos.distance(&enemy_pos);

        let actions = [
            (DirectedAction::MoveNorth, Position::new(player_pos.x, player_pos.y - 1)),
            (DirectedAction::MoveEast, Position::new(player_pos.x + 1, player_pos.y)),
            (DirectedAction::MoveSouth, Position::new(player_pos.x, player_pos.y + 1)),
            (DirectedAction::MoveWest, Position::new(player_pos.x - 1, player_pos.y)),
        ];

        for (action, new_pos) in actions {
            // Only consider walkable positions
            if !world.is_walkable(&new_pos, new_pos) {
                continue;
            }

            let dist = new_pos.distance(&enemy_pos);
            if dist > best_distance {
                best_distance = dist;
                best_action = Some(action);
            }
        }

        best_action.or(Some(DirectedAction::None))
    }
}

impl ExecuteGoal for AvoidEnemyGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        self.flee_direction(world, player_index)
    }
}
