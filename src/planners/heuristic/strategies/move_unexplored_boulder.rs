use tracing::debug;

use crate::planners::heuristic::goals::Goal;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::planners::heuristic::strategies::planner::{SelectGoal, StrategyType};

pub struct MoveUnexploredBoulderStrategy;

impl SelectGoal for MoveUnexploredBoulderStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, state, current_goals),
        fields(strategy = "MoveUnexploredBoulderStrategy")
    )]
    fn try_select_coop(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("MoveUnexploredBoulderStrategy");
        let mut goals = vec![None; state.world.players.len()];

        if state.world.level < 6 || state.world.boulders.is_empty() {
            return goals;
        }

        // Track which boulders have been assigned to prevent conflicts
        let mut assigned_boulders = std::collections::HashSet::new();

        for (player_index, player) in state.world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                continue;
            }
            if player.inventory != crate::swoq_interface::Inventory::None {
                continue;
            }

            debug!(
                "Player {} checking {} boulders for unexplored ones (frontier size: {})",
                player_index + 1,
                state.world.boulders.len(),
                player.unexplored_frontier.len()
            );

            // Check if any boulder is unexplored and reachable
            for boulder_pos in state.world.boulders.get_original_boulders() {
                // Skip if this boulder is already assigned to another player
                if assigned_boulders.contains(&boulder_pos) {
                    continue;
                }

                // Is the boulder unexplored (not moved by us)?
                if !state.world.boulders.has_moved(&boulder_pos) {
                    debug!("  Boulder at {:?} is unexplored", boulder_pos);

                    // Check if we can reach an adjacent position to pick it up
                    let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                        state.world.is_walkable(&adj, adj)
                            && state.world.find_path(player.position, adj).is_some()
                    });

                    if can_reach {
                        debug!(
                            "[MoveUnexploredBoulderStrategy] Player {} assigned boulder at {:?}",
                            player_index + 1,
                            boulder_pos
                        );
                        goals[player_index] = Some(Goal::FetchBoulder(boulder_pos));
                        assigned_boulders.insert(boulder_pos);
                        break;
                    } else {
                        debug!("  Boulder at {:?} is not reachable yet", boulder_pos);
                    }
                }
            }

            if goals[player_index].is_none() {
                debug!("Player {} found no reachable unexplored boulders", player_index + 1);
            }
        }

        goals
    }
}
