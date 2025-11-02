use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct PickupHealthStrategy;

impl SelectGoal for PickupHealthStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    #[tracing::instrument(level = "debug", skip(self, world, current_goals), fields(strategy = "PickupHealthStrategy"))]
    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("PickupHealthStrategy");
        if world.level < 10 || world.health.is_empty() {
            return vec![None; world.players.len()];
        }

        let mut goals = vec![None; world.players.len()];

        // Iterate over all health potions
        for health_pos in world.health.get_positions() {
            let mut best_player: Option<(usize, i32, usize)> = None; // (player_index, health, distance)

            // Find the best player for this specific health potion
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if current_goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already assigned a health pickup in this iteration
                if goals[player_index].is_some() {
                    continue;
                }

                // Check if any enemy is close (within 2 tiles actual path distance)
                let enemy_nearby = world.enemies.get_positions().iter().any(|&enemy_pos| {
                    world.path_distance_to_enemy(player.position, enemy_pos) <= 2
                });

                if enemy_nearby {
                    continue;
                }

                // Check if this player can reach this health potion
                if let Some(path) = world.find_path(player.position, *health_pos) {
                    let distance = path.len();
                    let should_select = match best_player {
                        None => true,
                        Some((_, best_health, best_distance)) => {
                            // Prefer player with lower health
                            if player.health < best_health {
                                true
                            } else if player.health == best_health {
                                // If equal health, prefer closer player
                                distance < best_distance
                            } else {
                                false
                            }
                        }
                    };

                    if should_select {
                        best_player = Some((player_index, player.health, distance));
                    }
                }
            }

            // Assign this health potion to the best player found
            if let Some((player_index, _, _)) = best_player {
                debug!(
                    "[PickupHealthStrategy] Player {} selected for PickupHealth (health={}, pos={:?})",
                    player_index + 1,
                    world.players[player_index].health,
                    health_pos
                );
                goals[player_index] = Some(Goal::PickupHealth(*health_pos));
            }
        }

        goals
    }
}
