use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct DropBoulderStrategy;

impl SelectGoal for DropBoulderStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 6 {
            return vec![None; world.players.len()];
        }

        let mut goals = Vec::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                goals.push(None);
                continue;
            }
            if player.inventory != crate::swoq_interface::Inventory::Boulder {
                goals.push(None);
                continue;
            }

            debug!("No pressure plates in level, need to drop boulder");
            goals.push(Some(Goal::DropBoulder));
        }

        goals
    }
}
