use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::Position;
use crate::world_state::WorldState;

pub struct RandomExploreStrategy;

impl SelectGoal for RandomExploreStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Individual
    }

    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let player = &world.players[player_index];

        // Only use random exploration when:
        // 1. The frontier is empty (nothing new to explore)
        // 2. We're not doing anything else
        if !player.unexplored_frontier.is_empty() {
            return None;
        }

        // If we already have a RandomExplore goal and destination, keep it
        if let Some(Goal::RandomExplore(_)) = &player.previous_goal
            && player.current_destination.is_some()
        {
            debug!("RandomExploreStrategy: Continuing with existing destination");
            return player.previous_goal.clone();
        }

        debug!("RandomExploreStrategy: Frontier empty, selecting random reachable position");

        // Collect all empty positions that we've seen
        let empty_positions: Vec<Position> = world
            .map
            .iter()
            .filter_map(|(pos, tile)| {
                if matches!(tile, crate::swoq_interface::Tile::Empty)
                    && player.position.distance(pos) > 5
                {
                    Some(*pos)
                } else {
                    None
                }
            })
            .collect();

        if empty_positions.is_empty() {
            debug!("RandomExploreStrategy: No empty positions found");
            return None;
        }

        // Try random positions until we find a reachable one (max 10 attempts)
        let mut seed = world.tick as usize;
        for _ in 0..10 {
            let index = seed % empty_positions.len();
            let target = empty_positions[index];

            // Check if reachable
            if world.map.find_path(player.position, target).is_some() {
                debug!("RandomExploreStrategy: Selected reachable position {:?}", target);
                return Some(Goal::RandomExplore(target));
            }

            // Try next position
            seed = seed.wrapping_add(1);
        }

        debug!("RandomExploreStrategy: No reachable position found after 10 attempts");
        None
    }
}
