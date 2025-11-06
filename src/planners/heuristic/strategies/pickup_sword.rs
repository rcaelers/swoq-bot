use tracing::debug;

use crate::planners::heuristic::goals::Goal;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::planners::heuristic::strategies::planner::{SelectGoal, StrategyType};

pub struct PickupSwordStrategy;

impl SelectGoal for PickupSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, state, current_goals),
        fields(strategy = "PickupSwordStrategy")
    )]
    fn try_select_coop(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("PickupSwordStrategy");
        if state.world.level < 10 || state.world.swords.is_empty() {
            return vec![None; state.world.players.len()];
        }

        let mut goals = vec![None; state.world.players.len()];

        // Iterate over all swords
        for sword_pos in state.world.swords.get_positions() {
            let mut best_player: Option<(usize, usize)> = None; // (player_index, distance)

            // Find the best player for this specific sword
            for (player_index, player) in state.world.players.iter().enumerate() {
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
                if let Some(path) = state.world.find_path(player.position, *sword_pos) {
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
                    state.world.players[player_index].has_sword,
                    sword_pos
                );
                goals[player_index] = Some(Goal::PickupSword);
            }
        }

        goals
    }
}
