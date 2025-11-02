use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct ReachExitStrategy;

impl SelectGoal for ReachExitStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Individual
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, world),
        fields(strategy = "ReachExitStrategy")
    )]
    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
        debug!("ReachExitStrategy: Checking for player {}", player_index + 1);
        let player = &world.players[player_index];

        let exit_pos = world.exit_position?;
        debug!("ReachExitStrategy: Exit at {:?}, player at {:?}", exit_pos, player.position);

        // For 2-player mode, check if all active players can reach the exit before anyone tries
        if world.players.len() > 1 {
            let active_players: Vec<&crate::player_state::PlayerState> =
                world.players.iter().filter(|p| p.is_active).collect();

            debug!("ReachExitStrategy: 2-player mode, {} active players", active_players.len());

            // If we have multiple active players, check if all can reach the exit
            if active_players.len() > 1 {
                let reachability: Vec<(usize, bool)> = active_players
                    .iter()
                    .enumerate()
                    .map(|(idx, p)| {
                        let carrying_boulder = p.inventory == crate::swoq_interface::Inventory::Boulder;
                        let can_reach = world.find_path(p.position, exit_pos).is_some();
                        debug!(
                            "ReachExitStrategy: Player {} - pos {:?}, carrying_boulder={}, can_reach={}",
                            idx + 1,
                            p.position,
                            carrying_boulder,
                            can_reach
                        );
                        (idx, carrying_boulder || can_reach)
                    })
                    .collect();

                let all_can_reach = reachability.iter().all(|(_, can_reach)| *can_reach);

                // If not all active players can reach the exit, don't assign exit goal to anyone
                if !all_can_reach {
                    debug!(
                        "ReachExitStrategy: Not all active players can reach exit, continuing exploration"
                    );
                    return None;
                } else {
                    debug!("ReachExitStrategy: All active players can reach exit, proceeding");
                }
            }
        }

        // Check if we're carrying a boulder - must drop it before exiting
        if player.inventory == crate::swoq_interface::Inventory::Boulder {
            debug!(
                "ReachExitStrategy: Player {} carrying boulder, must drop before exiting",
                player_index + 1
            );
            return Some(Goal::DropBoulder);
        }

        // Check if we can actually path to the exit
        let can_path_to_exit = world.find_path(player.position, exit_pos).is_some();
        debug!(
            "ReachExitStrategy: Player {} can_path_to_exit={}",
            player_index + 1,
            can_path_to_exit
        );

        if can_path_to_exit {
            debug!("ReachExitStrategy: Assigning ReachExit goal to player {}", player_index + 1);
            Some(Goal::ReachExit)
        } else {
            debug!(
                "ReachExitStrategy: Exit at {:?} is not reachable for player {}, continuing exploration",
                exit_pos,
                player_index + 1
            );
            None
        }
    }
}
