use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct PickupSwordStrategy;

impl SelectGoal for PickupSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 10 || world.swords.is_empty() {
            return vec![None; world.players.len()];
        }

        let mut goals = vec![None; world.players.len()];

        // Iterate over all swords
        for sword_pos in world.swords.get_positions() {
            let mut best_player: Option<(usize, usize)> = None; // (player_index, distance)

            // Find the best player for this specific sword
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if current_goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already assigned a sword pickup in this iteration
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already has a sword
                if player.has_sword {
                    continue;
                }

                // Check if this player can reach this sword
                if let Some(path) = world.find_path(player.position, *sword_pos) {
                    let distance = path.len();
                    let should_select = match best_player {
                        None => true,
                        Some((_, best_distance)) => {
                            // Prefer closer player
                            distance < best_distance
                        }
                    };

                    if should_select {
                        best_player = Some((player_index, distance));
                    }
                }
            }

            // Assign this sword to the best player found
            if let Some((player_index, _)) = best_player {
                debug!(
                    "[PickupSwordStrategy] Player {} selected for PickupSword (has_sword={}, sword_pos={:?})",
                    player_index + 1,
                    world.players[player_index].has_sword,
                    sword_pos
                );
                goals[player_index] = Some(Goal::PickupSword);
            }
        }

        goals
    }
}
