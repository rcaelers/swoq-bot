use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::Color;
use crate::world_state::WorldState;

pub struct FetchBoulderForPlateStrategy;

impl SelectGoal for FetchBoulderForPlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        if world.level < 6 || world.boulders.is_empty() {
            return goals;
        }

        // Track which boulders have been assigned to prevent conflicts
        let mut assigned_boulders = std::collections::HashSet::new();

        // Iterate over each color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Only consider this color if there's a door and pressure plate
            if !world.doors.has_color(color) || !world.pressure_plates.has_color(color) {
                continue;
            }

            // Check if any door of this color is reachable by any player
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            debug!("Checking {:?} color for boulder fetch (has doors and plates)", color);

            let mut best_player: Option<(usize, i32, crate::types::Position)> = None; // (player_index, distance, boulder_pos)

            // Find the best player to fetch a boulder for this color
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal (from current goals or reused goal)
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }
                if player.inventory != crate::swoq_interface::Inventory::None {
                    continue;
                }

                // Check if this player can reach any door of this color
                let can_reach_door = door_positions.iter().any(|&door_pos| {
                    door_pos.neighbors().iter().any(|&neighbor| {
                        matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                            && world.map.find_path(player.position, neighbor).is_some()
                    })
                });

                if !can_reach_door {
                    continue;
                }

                // First, check if this player has an existing FetchBoulder goal that's still valid
                if let Some(Goal::FetchBoulder(boulder_pos)) = &player.previous_goal {
                    // Check if this boulder still exists and hasn't been assigned
                    if world.boulders.get_all_positions().contains(boulder_pos)
                        && !assigned_boulders.contains(boulder_pos)
                    {
                        // Verify the boulder is still reachable
                        let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                            world.map.is_walkable(&adj, adj)
                                && world.map.find_path(player.position, adj).is_some()
                        });

                        if can_reach {
                            debug!(
                                "[FetchBoulderForPlateStrategy] Player {} reusing existing FetchBoulder goal for boulder at {:?} (conditions still valid)",
                                player_index + 1,
                                boulder_pos
                            );
                            goals[player_index] = Some(Goal::FetchBoulder(*boulder_pos));
                            assigned_boulders.insert(*boulder_pos);
                            continue;
                        }
                    }
                }

                // Find the closest reachable boulder that hasn't been assigned
                let mut closest_boulder = None;
                let mut min_distance = i32::MAX;

                for boulder_pos in world.boulders.get_all_positions() {
                    // Skip if this boulder is already assigned
                    if assigned_boulders.contains(&boulder_pos) {
                        continue;
                    }

                    // Check if we can reach an adjacent position to pick it up
                    let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                        world.map.is_walkable(&adj, adj)
                            && world.map.find_path(player.position, adj).is_some()
                    });

                    if can_reach {
                        let dist = player.position.distance(&boulder_pos);
                        if dist < min_distance {
                            min_distance = dist;
                            closest_boulder = Some(boulder_pos);
                        }
                    }
                }

                // If we found a reachable boulder, consider this player
                if let Some(boulder_pos) = closest_boulder {
                    // Update best player if this one is closer to their boulder
                    best_player = match best_player {
                        None => Some((player_index, min_distance, boulder_pos)),
                        Some((_, best_distance, _)) if min_distance < best_distance => {
                            Some((player_index, min_distance, boulder_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player
            if let Some((player_index, _, boulder_pos)) = best_player {
                debug!(
                    "[FetchBoulderForPlateStrategy] Player {} assigned to fetch boulder at {:?} for {:?} pressure plate (has matching reachable door)",
                    player_index + 1,
                    boulder_pos,
                    color
                );
                goals[player_index] = Some(Goal::FetchBoulder(boulder_pos));
                assigned_boulders.insert(boulder_pos);
            }
        }

        goals
    }
}
