use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct HuntEnemyWithSwordStrategy;

impl SelectGoal for HuntEnemyWithSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Individual
    }

    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let player = &world.players[player_index];

        // Only hunt enemies when:
        // 1. We have a sword
        // 2. The entire maze is explored (frontier is empty)
        // 3. There are enemies or potential enemy locations
        debug!(
            "HuntEnemyWithSwordStrategy check: has_sword={}, frontier_empty={}, enemies_present={} (count={}), potential_enemies={} (count={})",
            player.has_sword,
            player.unexplored_frontier.is_empty(),
            !world.enemies.is_empty(),
            world.enemies.get_positions().len(),
            !world.potential_enemy_locations.is_empty(),
            world.potential_enemy_locations.len()
        );

        if !player.has_sword
            || !player.unexplored_frontier.is_empty()
            || (world.enemies.is_empty() && world.potential_enemy_locations.is_empty())
        {
            return None;
        }

        debug!("Maze fully explored, have sword, hunting enemy (may drop key)");

        // Find the closest enemy
        if let Some(enemy_pos) = world.closest_enemy(player) {
            debug!("Hunting known enemy at {:?}", enemy_pos);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        // If no known enemies, hunt potential enemy locations
        if let Some(potential_pos) = world.closest_potential_enemy(player) {
            debug!("No known enemies, hunting potential enemy location at {:?}", potential_pos);
            return Some(Goal::KillEnemy(potential_pos));
        }

        None
    }
}
