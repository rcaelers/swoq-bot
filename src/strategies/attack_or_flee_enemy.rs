use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct AttackOrFleeEnemyStrategy;

impl SelectGoal for AttackOrFleeEnemyStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Individual
    }

    #[tracing::instrument(level = "debug", skip(self, world), fields(strategy = "AttackOrFleeEnemyStrategy"))]
    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
        debug!("AttackOrFleeEnemyStrategy");
        if world.level < 8 {
            return None;
        }

        let player = &world.players[player_index];
        let enemy_pos = world.closest_enemy(player)?;
        let dist = world.path_distance_to_enemy(player.position, enemy_pos);

        // If we have a sword and enemy is close (adjacent or 2 tiles away), attack it
        if player.has_sword && dist <= 2 {
            debug!("(have sword, enemy within {} tiles)", dist);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        // If we don't have sword and enemy is dangerously close, flee
        if dist <= 3 && !player.has_sword {
            return Some(Goal::AvoidEnemy(enemy_pos));
        }

        None
    }
}
