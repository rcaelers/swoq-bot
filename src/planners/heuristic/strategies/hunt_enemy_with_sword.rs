use std::collections::HashSet;
use tracing::debug;

use crate::planners::heuristic::goals::Goal;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::planners::heuristic::strategies::planner::{SelectGoal, StrategyType};
use crate::infra::Position;

pub struct HuntEnemyWithSwordStrategy;

impl HuntEnemyWithSwordStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl SelectGoal for HuntEnemyWithSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, state, current_goals),
        fields(strategy = "HuntEnemyWithSwordStrategy")
    )]
    fn try_select_coop(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("HuntEnemyWithSwordStrategy: Starting enemy hunt evaluation");

        let mut goals = vec![None; state.world.players.len()];

        // Track which enemies are already being targeted
        let mut targeted_enemies: HashSet<Position> = HashSet::new();
        for goal in current_goals.iter().flatten() {
            if let Goal::KillEnemy(pos) = goal {
                targeted_enemies.insert(*pos);
            }
        }

        debug!("HuntEnemyWithSwordStrategy: Already targeted enemies: {:?}", targeted_enemies);

        for (player_index, player) in state.world.players.iter().enumerate() {
            if !player.is_active {
                continue;
            }

            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                debug!(
                    "HuntEnemyWithSwordStrategy: Player {} already has a goal",
                    player_index + 1
                );
                continue;
            }

            // Only hunt enemies when:
            // 1. We have a sword
            // 2. The entire maze is explored (frontier is empty)
            // 3. There are enemies or potential enemy locations
            // 4. Player health > 2 (don't hunt if health is too low)
            debug!(
                "HuntEnemyWithSwordStrategy: Player {} check: has_sword={}, health={}, frontier_empty={}, enemies_present={} (count={}), potential_enemies={} (count={})",
                player_index + 1,
                player.has_sword,
                player.health,
                player.unexplored_frontier.is_empty(),
                !state.world.enemies.is_empty(),
                state.world.enemies.get_positions().len(),
                !state.world.potential_enemy_locations.is_empty(),
                state.world.potential_enemy_locations.len()
            );

            if !player.has_sword
                || player.health <= 2
                || !player.unexplored_frontier.is_empty()
                || (state.world.enemies.is_empty()
                    && state.world.potential_enemy_locations.is_empty())
            {
                if player.has_sword && player.health <= 2 {
                    debug!(
                        "HuntEnemyWithSwordStrategy: Player {} health too low ({} <= 2), not hunting",
                        player_index + 1,
                        player.health
                    );
                }
                continue;
            }

            debug!(
                "HuntEnemyWithSwordStrategy: Player {} - maze fully explored, have sword, health={}, hunting enemy (may drop key)",
                player_index + 1,
                player.health
            );

            // Find the closest enemy that isn't already targeted
            if let Some(enemy_pos) = state.world.closest_enemy(player) {
                if !targeted_enemies.contains(&enemy_pos) {
                    debug!(
                        "HuntEnemyWithSwordStrategy: Player {} hunting known enemy at {:?}",
                        player_index + 1,
                        enemy_pos
                    );
                    goals[player_index] = Some(Goal::KillEnemy(enemy_pos));
                    targeted_enemies.insert(enemy_pos);
                    continue;
                } else {
                    debug!(
                        "HuntEnemyWithSwordStrategy: Player {} skipping enemy at {:?} (already targeted)",
                        player_index + 1,
                        enemy_pos
                    );
                }
            }

            // If no known enemies (or all are targeted), hunt potential enemy locations
            if let Some(potential_pos) = state.world.closest_potential_enemy(player) {
                if !targeted_enemies.contains(&potential_pos) {
                    debug!(
                        "HuntEnemyWithSwordStrategy: Player {} hunting potential enemy location at {:?}",
                        player_index + 1,
                        potential_pos
                    );
                    goals[player_index] = Some(Goal::KillEnemy(potential_pos));
                    targeted_enemies.insert(potential_pos);
                } else {
                    debug!(
                        "HuntEnemyWithSwordStrategy: Player {} skipping potential enemy at {:?} (already targeted)",
                        player_index + 1,
                        potential_pos
                    );
                }
            }
        }

        debug!("HuntEnemyWithSwordStrategy: Final goals: {:?}", goals);
        goals
    }
}
