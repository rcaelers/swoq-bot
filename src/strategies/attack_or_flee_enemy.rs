use std::collections::HashSet;
use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::Position;
use crate::world_state::WorldState;

pub struct AttackOrFleeEnemyStrategy;

impl SelectGoal for AttackOrFleeEnemyStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, world, current_goals),
        fields(strategy = "AttackOrFleeEnemyStrategy")
    )]
    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("AttackOrFleeEnemyStrategy: Starting enemy evaluation");

        if world.level < 8 {
            debug!("AttackOrFleeEnemyStrategy: Level < 8, skipping");
            return vec![None; world.players.len()];
        }

        let mut goals = vec![None; world.players.len()];

        // Track which enemies are already being targeted by current goals
        let mut targeted_enemies: HashSet<Position> = HashSet::new();
        for goal in current_goals.iter().flatten() {
            if let Goal::KillEnemy(pos) = goal {
                targeted_enemies.insert(*pos);
            }
        }

        debug!("AttackOrFleeEnemyStrategy: Already targeted enemies: {:?}", targeted_enemies);

        for (player_index, player) in world.players.iter().enumerate() {
            if !player.is_active {
                continue;
            }

            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                debug!("AttackOrFleeEnemyStrategy: Player {} already has a goal", player_index + 1);
                continue;
            }

            let enemy_pos = match world.closest_enemy(player) {
                Some(pos) => pos,
                None => {
                    debug!(
                        "AttackOrFleeEnemyStrategy: Player {} has no enemies nearby",
                        player_index + 1
                    );
                    continue;
                }
            };

            let dist = world.path_distance_to_enemy(player.position, enemy_pos);
            debug!(
                "AttackOrFleeEnemyStrategy: Player {} at {:?}, closest enemy at {:?}, distance={}",
                player_index + 1,
                player.position,
                enemy_pos,
                dist
            );

            // If we have a sword and enemy is close (adjacent or 2 tiles away), attack it
            if player.has_sword && dist <= 2 {
                // Only assign if no one else is already targeting this enemy
                if !targeted_enemies.contains(&enemy_pos) {
                    debug!(
                        "AttackOrFleeEnemyStrategy: Player {} attacking enemy at {:?} (distance={})",
                        player_index + 1,
                        enemy_pos,
                        dist
                    );
                    goals[player_index] = Some(Goal::KillEnemy(enemy_pos));
                    targeted_enemies.insert(enemy_pos);
                } else {
                    debug!(
                        "AttackOrFleeEnemyStrategy: Player {} skipping enemy at {:?} (already targeted)",
                        player_index + 1,
                        enemy_pos
                    );
                }
                continue;
            }

            // If we don't have sword and enemy is dangerously close, flee
            if dist <= 3 && !player.has_sword {
                debug!(
                    "AttackOrFleeEnemyStrategy: Player {} fleeing from enemy at {:?} (no sword, distance={})",
                    player_index + 1,
                    enemy_pos,
                    dist
                );
                goals[player_index] = Some(Goal::AvoidEnemy(enemy_pos));
            }
        }

        debug!("AttackOrFleeEnemyStrategy: Final goals: {:?}", goals);
        goals
    }
}
