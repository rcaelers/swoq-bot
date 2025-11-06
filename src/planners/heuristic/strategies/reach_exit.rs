use tracing::debug;

use crate::planners::heuristic::goals::Goal;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::planners::heuristic::strategies::planner::{SelectGoal, StrategyType};

pub struct ReachExitStrategy;

impl SelectGoal for ReachExitStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, state),
        fields(strategy = "ReachExitStrategy")
    )]
    fn try_select_coop(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; state.world.players.len()];

        let exit_pos = match state.world.exit_position {
            Some(pos) => pos,
            None => return goals,
        };

        debug!("ReachExitStrategy: Exit at {:?}", exit_pos);

        // For 2-player mode, check if all active players can reach the exit before anyone tries
        if state.world.players.len() > 1 {
            let active_players: Vec<(usize, &crate::state::PlayerState)> = state
                .world
                .players
                .iter()
                .enumerate()
                .filter(|(_, p)| p.is_active)
                .collect();

            debug!("ReachExitStrategy: 2-player mode, {} active players", active_players.len());

            // If we have multiple active players, check if all can reach the exit
            if active_players.len() > 1 {
                let reachability: Vec<(usize, bool)> = active_players
                    .iter()
                    .map(|(idx, p)| {
                        let carrying_boulder = p.inventory == crate::swoq_interface::Inventory::Boulder;
                        let can_reach = state.world.find_path(p.position, exit_pos).is_some();
                        debug!(
                            "ReachExitStrategy: Player {} - pos {:?}, carrying_boulder={}, can_reach={}",
                            idx + 1,
                            p.position,
                            carrying_boulder,
                            can_reach
                        );
                        (*idx, carrying_boulder || can_reach)
                    })
                    .collect();

                let all_can_reach = reachability.iter().all(|(_, can_reach)| *can_reach);

                // If not all active players can reach the exit, don't assign exit goal to anyone
                if !all_can_reach {
                    debug!(
                        "ReachExitStrategy: Not all active players can reach exit, continuing exploration"
                    );
                    return goals;
                }

                debug!(
                    "ReachExitStrategy: All active players can reach exit, assigning ReachExit to all"
                );

                // Check if any player already has a goal - if so, we can't assign ReachExit to anyone
                let any_player_has_goal = active_players
                    .iter()
                    .any(|(idx, _)| current_goals[*idx].is_some());
                if any_player_has_goal {
                    debug!(
                        "ReachExitStrategy: At least one player already has a goal, cannot assign ReachExit"
                    );
                    return goals;
                }

                // Assign goals to all active players
                for (player_idx, player) in state.world.players.iter().enumerate() {
                    if !player.is_active {
                        continue;
                    }

                    // Check if we're carrying a boulder - must drop it before exiting
                    if player.inventory == crate::swoq_interface::Inventory::Boulder {
                        debug!(
                            "ReachExitStrategy: Player {} carrying boulder, must drop before exiting",
                            player_idx + 1
                        );
                        goals[player_idx] = Some(Goal::DropBoulder);
                    } else {
                        debug!(
                            "ReachExitStrategy: Assigning ReachExit goal to player {}",
                            player_idx + 1
                        );
                        goals[player_idx] = Some(Goal::ReachExit);
                    }
                }

                return goals;
            }
        }

        // Single player mode or only one active player - check individually
        for (player_idx, player) in state.world.players.iter().enumerate() {
            if !player.is_active || current_goals[player_idx].is_some() {
                continue;
            }

            debug!("ReachExitStrategy: Checking player {}", player_idx + 1);

            // Check if we're carrying a boulder - must drop it before exiting
            if player.inventory == crate::swoq_interface::Inventory::Boulder {
                debug!(
                    "ReachExitStrategy: Player {} carrying boulder, must drop before exiting",
                    player_idx + 1
                );
                goals[player_idx] = Some(Goal::DropBoulder);
                continue;
            }

            // Check if we can actually path to the exit
            let can_path_to_exit = state.world.find_path(player.position, exit_pos).is_some();
            debug!(
                "ReachExitStrategy: Player {} can_path_to_exit={}",
                player_idx + 1,
                can_path_to_exit
            );

            if can_path_to_exit {
                debug!("ReachExitStrategy: Assigning ReachExit goal to player {}", player_idx + 1);
                goals[player_idx] = Some(Goal::ReachExit);
            } else {
                debug!(
                    "ReachExitStrategy: Exit at {:?} is not reachable for player {}, continuing exploration",
                    exit_pos,
                    player_idx + 1
                );
            }
        }

        goals
    }
}
