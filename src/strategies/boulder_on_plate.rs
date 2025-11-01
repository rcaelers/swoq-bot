use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::{Color, Position};
use crate::world_state::WorldState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoulderPlatePhase {
    FetchBoulder(Position), // Position of boulder to fetch
    DropOnPlate(Position),  // Position of plate to drop on
}

#[derive(Debug, Clone)]
struct ColorAssignment {
    player_index: usize,
    color: Color,
    phase: BoulderPlatePhase,
}

pub struct BoulderOnPlateStrategy {
    // Track which colors are assigned to which players
    assignments: Vec<ColorAssignment>,
}

impl BoulderOnPlateStrategy {
    pub fn new() -> Self {
        Self {
            assignments: Vec::new(),
        }
    }

    fn clean_assignments(&mut self, world: &WorldState) -> Vec<usize> {
        // Track which players had assignments removed and still have boulders
        let mut players_to_drop_boulder = Vec::new();

        self.assignments.retain(|assignment| {
            let should_remove = {
                // Remove if door no longer exists
                if !world.doors.has_color(assignment.color) {
                    debug!(
                        "Removing boulder/plate assignment for {:?}: door no longer exists",
                        assignment.color
                    );
                    true
                // Remove if door has been opened (by key or by pressure plate with boulder)
                } else if world.has_door_been_opened(assignment.color) {
                    debug!(
                        "Removing boulder/plate assignment for {:?}: door has been opened",
                        assignment.color
                    );
                    true
                // Remove if pressure plates no longer exist for this color
                } else if !world.pressure_plates.has_color(assignment.color) {
                    debug!(
                        "Removing boulder/plate assignment for {:?}: pressure plates gone",
                        assignment.color
                    );
                    true
                // Remove if player no longer exists or is inactive
                } else if assignment.player_index >= world.players.len()
                    || !world.players[assignment.player_index].is_active
                {
                    debug!(
                        "Removing boulder/plate assignment for {:?}: player {} no longer active",
                        assignment.color,
                        assignment.player_index + 1
                    );
                    true
                // Remove if in FetchBoulder phase and boulder no longer exists
                // BUT only if the player doesn't have a boulder (if they do, update_phases will handle it)
                } else if let BoulderPlatePhase::FetchBoulder(boulder_pos) = assignment.phase {
                    let player = &world.players[assignment.player_index];
                    if !world.boulders.get_all_positions().contains(&boulder_pos)
                        && player.inventory != crate::swoq_interface::Inventory::Boulder
                    {
                        debug!(
                            "Removing boulder/plate assignment for {:?}: boulder at {:?} no longer exists and player doesn't have it",
                            assignment.color, boulder_pos
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            // If removing this assignment and player still has a boulder, mark for drop
            if should_remove
                && assignment.player_index < world.players.len()
                && world.players[assignment.player_index].inventory
                    == crate::swoq_interface::Inventory::Boulder
            {
                debug!(
                    "Player {} assignment removed but still carrying boulder - will drop it",
                    assignment.player_index + 1
                );
                players_to_drop_boulder.push(assignment.player_index);
            }

            !should_remove
        });

        players_to_drop_boulder
    }

    fn update_phases(&mut self, world: &WorldState) {
        for assignment in self.assignments.iter_mut() {
            let player = &world.players[assignment.player_index];

            // If in FetchBoulder phase and player now has boulder, advance to DropOnPlate
            if let BoulderPlatePhase::FetchBoulder(_) = assignment.phase
                && player.inventory == crate::swoq_interface::Inventory::Boulder
            {
                // Find closest reachable plate for this color
                if let Some(plates) = world.pressure_plates.get_positions(assignment.color) {
                    let mut closest_plate = None;
                    let mut min_distance = i32::MAX;

                    for &plate_pos in plates {
                        if let Some(dist) = world.path_distance(player.position, plate_pos)
                            && dist < min_distance
                        {
                            min_distance = dist;
                            closest_plate = Some(plate_pos);
                        }
                    }

                    if let Some(plate_pos) = closest_plate {
                        debug!(
                            "Player {} acquired boulder, advancing to DropOnPlate phase for {:?} at {:?}",
                            assignment.player_index + 1,
                            assignment.color,
                            plate_pos
                        );
                        assignment.phase = BoulderPlatePhase::DropOnPlate(plate_pos);
                    } else {
                        debug!(
                            "Player {} has boulder but no reachable {:?} plate found",
                            assignment.player_index + 1,
                            assignment.color
                        );
                    }
                }
            }
        }
    }

    fn assign_new_colors(&mut self, world: &WorldState) {
        if world.level < 6 || world.boulders.is_empty() {
            return;
        }

        // Get assigned colors and players
        let assigned_colors: std::collections::HashSet<Color> =
            self.assignments.iter().map(|a| a.color).collect();
        let assigned_players: std::collections::HashSet<usize> =
            self.assignments.iter().map(|a| a.player_index).collect();

        // Get colors that need assignment (have door, plates, and aren't opened)
        let unassigned_colors: Vec<Color> = [Color::Red, Color::Green, Color::Blue]
            .iter()
            .filter(|&&color| {
                !assigned_colors.contains(&color)
                    && world.doors.has_color(color)
                    && world.pressure_plates.has_color(color)
                    && !world.has_door_been_opened(color)
            })
            .copied()
            .collect();

        for color in unassigned_colors {
            // Check if any door of this color is reachable
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            let door_reachable_by_any_player = world.players.iter().any(|player| {
                door_positions.iter().any(|&door_pos| {
                    door_pos.neighbors().iter().any(|&neighbor| {
                        matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                            && world.find_path(player.position, neighbor).is_some()
                    })
                })
            });

            if !door_reachable_by_any_player {
                continue;
            }

            debug!("Checking {:?} color for boulder/plate assignment", color);

            // Find the best available player for this color
            let mut best_player: Option<(usize, i32, Position)> = None; // (player_index, distance, boulder_pos)

            for (player_index, player) in world.players.iter().enumerate() {
                if !player.is_active
                    || assigned_players.contains(&player_index)
                    || player.inventory != crate::swoq_interface::Inventory::None
                {
                    continue;
                }

                // Find closest reachable boulder
                let mut closest_boulder = None;
                let mut min_distance = i32::MAX;

                for boulder_pos in world.boulders.get_all_positions() {
                    // Check if we can reach an adjacent position to pick it up
                    let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                        world.is_walkable(&adj, adj)
                            && world.find_path(player.position, adj).is_some()
                    });

                    if can_reach {
                        let dist = player.position.distance(&boulder_pos);
                        if dist < min_distance {
                            min_distance = dist;
                            closest_boulder = Some(boulder_pos);
                        }
                    }
                }

                if let Some(boulder_pos) = closest_boulder {
                    best_player = match best_player {
                        None => Some((player_index, min_distance, boulder_pos)),
                        Some((_, best_distance, _)) if min_distance < best_distance => {
                            Some((player_index, min_distance, boulder_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the color to the best player
            if let Some((player_index, _, boulder_pos)) = best_player {
                debug!(
                    "[BoulderOnPlateStrategy] Assigning {:?} to player {} (FetchBoulder phase, target: {:?})",
                    color,
                    player_index + 1,
                    boulder_pos
                );
                self.assignments.push(ColorAssignment {
                    player_index,
                    color,
                    phase: BoulderPlatePhase::FetchBoulder(boulder_pos),
                });
            }
        }
    }

    fn generate_goals(
        &self,
        world: &WorldState,
        players_to_drop_boulder: &[usize],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Track which players have assignments
        let assigned_players: std::collections::HashSet<usize> =
            self.assignments.iter().map(|a| a.player_index).collect();

        // First, handle players who need to drop boulders (removed assignments)
        for &player_index in players_to_drop_boulder {
            if player_index < world.players.len()
                && world.players[player_index].inventory
                    == crate::swoq_interface::Inventory::Boulder
            {
                debug!(
                    "Player {} assigned DropBoulder goal (assignment removed but still carrying boulder)",
                    player_index + 1
                );
                goals[player_index] = Some(Goal::DropBoulder);
            }
        }

        // Handle players with assignments
        for assignment in &self.assignments {
            // Skip if player already has a drop goal
            if goals[assignment.player_index].is_some() {
                continue;
            }

            let player = &world.players[assignment.player_index];

            let goal = match assignment.phase {
                BoulderPlatePhase::FetchBoulder(boulder_pos) => {
                    // Verify boulder still exists and is reachable
                    if world.boulders.get_all_positions().contains(&boulder_pos) {
                        let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                            world.is_walkable(&adj, adj)
                                && world.find_path(player.position, adj).is_some()
                        });

                        if can_reach {
                            Some(Goal::FetchBoulder(boulder_pos))
                        } else {
                            debug!(
                                "Player {} cannot reach boulder at {:?} anymore",
                                assignment.player_index + 1,
                                boulder_pos
                            );
                            None
                        }
                    } else {
                        None
                    }
                }
                BoulderPlatePhase::DropOnPlate(plate_pos) => {
                    // Verify player still has boulder and plate is reachable
                    if player.inventory == crate::swoq_interface::Inventory::Boulder {
                        if world.path_distance(player.position, plate_pos).is_some() {
                            Some(Goal::DropBoulderOnPlate(assignment.color, plate_pos))
                        } else {
                            debug!(
                                "Player {} cannot reach {:?} plate at {:?} anymore",
                                assignment.player_index + 1,
                                assignment.color,
                                plate_pos
                            );
                            None
                        }
                    } else {
                        debug!(
                            "Player {} no longer has boulder for {:?} plate",
                            assignment.player_index + 1,
                            assignment.color
                        );
                        None
                    }
                }
            };

            if let Some(g) = goal {
                goals[assignment.player_index] = Some(g);
            }
        }

        // Finally, handle unassigned players carrying boulders (fallback - no pressure plates available)
        if world.level >= 6 {
            for (player_index, player) in world.players.iter().enumerate() {
                if goals[player_index].is_none()
                    && !assigned_players.contains(&player_index)
                    && player.inventory == crate::swoq_interface::Inventory::Boulder
                {
                    debug!(
                        "Player {} carrying boulder with no assignment - will drop it (no pressure plates available)",
                        player_index + 1
                    );
                    goals[player_index] = Some(Goal::DropBoulder);
                }
            }
        }

        goals
    }
}

impl SelectGoal for BoulderOnPlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        // Clean up invalid assignments and get list of players who need to drop boulders
        let players_to_drop_boulder = self.clean_assignments(world);

        // Update phases based on player inventory
        self.update_phases(world);

        // Try to assign new colors to available players
        self.assign_new_colors(world);

        // Generate goals based on current assignments
        let goals = self.generate_goals(world, &players_to_drop_boulder);

        // Only return goals for players that don't already have one
        goals
            .into_iter()
            .enumerate()
            .map(|(i, goal)| {
                if i < current_goals.len() && current_goals[i].is_some() {
                    None
                } else {
                    goal
                }
            })
            .collect()
    }
}
