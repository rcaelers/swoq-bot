use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct ReachExitStrategy;

impl SelectGoal for ReachExitStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Individual
    }

    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let player = &world.players[player_index];

        let exit_pos = world.exit_position?;

        // For 2-player mode, check if all active players can reach the exit before anyone tries
        if world.players.len() > 1 {
            let active_players: Vec<&crate::player_state::PlayerState> =
                world.players.iter().filter(|p| p.is_active).collect();

            // If we have multiple active players, check if all can reach the exit
            if active_players.len() > 1 {
                let all_can_reach = active_players.iter().all(|p| {
                    p.inventory == crate::swoq_interface::Inventory::Boulder
                        || world.find_path(p.position, exit_pos).is_some()
                });

                // If not all active players can reach the exit, don't assign exit goal to anyone
                if !all_can_reach {
                    debug!(
                        "2-player mode: Not all active players can reach exit, continuing exploration"
                    );
                    return None;
                }
            }
        }

        // Check if we're carrying a boulder - must drop it before exiting
        if player.inventory == crate::swoq_interface::Inventory::Boulder {
            debug!("Need to drop boulder before reaching exit");
            return Some(Goal::DropBoulder);
        }

        // Check if we can actually path to the exit
        if world.find_path(player.position, exit_pos).is_some() {
            Some(Goal::ReachExit)
        } else {
            debug!("Exit at {:?} is not reachable, continuing exploration", exit_pos);
            None
        }
    }
}
