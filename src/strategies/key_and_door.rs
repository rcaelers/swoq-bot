use std::collections::HashMap;
use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::Color;
use crate::world_state::WorldState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyDoorPhase {
    FetchKey,
    OpenDoor,
}

#[derive(Debug, Clone)]
struct ColorAssignment {
    player_index: usize,
    phase: KeyDoorPhase,
}

pub struct KeyAndDoorStrategy {
    // Track which colors are assigned to which players
    color_assignments: HashMap<Color, ColorAssignment>,
}

impl KeyAndDoorStrategy {
    pub fn new() -> Self {
        Self {
            color_assignments: HashMap::new(),
        }
    }

    fn clean_assignments(&mut self, world: &WorldState) {
        // Remove assignments for colors where the door has been opened or is no longer relevant
        self.color_assignments.retain(|color, assignment| {
            // Keep if door still exists and hasn't been opened
            if !world.doors.has_color(*color) {
                debug!("Removing assignment for {:?}: door no longer exists", color);
                return false;
            }

            // Check if door has been opened (by key or by pressure plate with boulder)
            if world.has_door_been_opened(*color) {
                debug!("Removing assignment for {:?}: door has been opened", color);
                return false;
            }

            // Check if assigned player still exists and is active
            if assignment.player_index >= world.players.len()
                || !world.players[assignment.player_index].is_active
            {
                debug!(
                    "Removing assignment for {:?}: player {} no longer active",
                    color,
                    assignment.player_index + 1
                );
                return false;
            }

            true
        });
    }

    fn update_phases(&mut self, world: &WorldState) {
        // Update phases based on current player inventory
        for (color, assignment) in self.color_assignments.iter_mut() {
            let player = &world.players[assignment.player_index];

            // If player has the key, advance to OpenDoor phase
            if assignment.phase == KeyDoorPhase::FetchKey && world.has_key(player, *color) {
                debug!(
                    "Player {} has acquired {:?} key, advancing to OpenDoor phase",
                    assignment.player_index + 1,
                    color
                );
                assignment.phase = KeyDoorPhase::OpenDoor;
            }
        }
    }

    fn assign_new_colors(&mut self, world: &WorldState) {
        // Get colors that need assignment
        let unassigned_colors: Vec<Color> = world
            .doors
            .colors()
            .filter(|color| {
                !self.color_assignments.contains_key(color) && !world.has_door_been_opened(**color)
            })
            .copied()
            .collect();

        // In 2-player mode, check which door colors have reachable pressure plates
        let mut doors_with_plates = std::collections::HashSet::new();
        if world.is_two_player_mode() {
            for &color in &[Color::Red, Color::Green, Color::Blue] {
                if let Some(plates) = world.pressure_plates.get_positions(color) {
                    let can_reach_plate = world.players.iter().any(|player| {
                        plates
                            .iter()
                            .any(|&plate_pos| world.find_path(player.position, plate_pos).is_some())
                    });
                    if can_reach_plate {
                        doors_with_plates.insert(color);
                        debug!("In 2-player mode: {:?} door has reachable pressure plate", color);
                    }
                }
            }
        }

        // Find available players (not already assigned to a color)
        let assigned_players: std::collections::HashSet<usize> = self
            .color_assignments
            .values()
            .map(|a| a.player_index)
            .collect();

        for color in unassigned_colors {
            // Check if we know where the key is
            if !world.knows_key_location(color) {
                continue;
            }

            // Find the best available player for this color
            let mut best_player: Option<(usize, i32)> = None;

            for (player_index, player) in world.players.iter().enumerate() {
                if !player.is_active || assigned_players.contains(&player_index) {
                    continue;
                }

                if let Some(key_pos) = world.closest_key(player, color) {
                    // In 2-player mode with reachable pressure plate, treat matching doors as walkable
                    let can_reach = if world.is_two_player_mode()
                        && doors_with_plates.contains(&color)
                    {
                        world
                            .find_path_with_custom_walkability(
                                player.position,
                                key_pos,
                                |pos, goal, _tick| {
                                    let is_matching_door = matches!(
                                        (world.map.get(pos), color),
                                        (Some(crate::swoq_interface::Tile::DoorRed), Color::Red)
                                            | (
                                                Some(crate::swoq_interface::Tile::DoorGreen),
                                                Color::Green
                                            )
                                            | (
                                                Some(crate::swoq_interface::Tile::DoorBlue),
                                                Color::Blue
                                            )
                                    );
                                    if is_matching_door {
                                        true
                                    } else {
                                        world.is_walkable(pos, goal)
                                    }
                                },
                            )
                            .is_some()
                    } else {
                        world.find_path(player.position, key_pos).is_some()
                    };

                    if can_reach {
                        let distance = player.position.distance(&key_pos);
                        best_player = match best_player {
                            None => Some((player_index, distance)),
                            Some((_, best_dist)) if distance < best_dist => {
                                Some((player_index, distance))
                            }
                            _ => best_player,
                        };
                    }
                }
            }

            // Assign the color to the best player
            if let Some((player_index, _)) = best_player {
                debug!(
                    "[KeyAndDoorStrategy] Assigning {:?} to player {} (FetchKey phase)",
                    color,
                    player_index + 1
                );
                self.color_assignments.insert(
                    color,
                    ColorAssignment {
                        player_index,
                        phase: KeyDoorPhase::FetchKey,
                    },
                );
            }
        }
    }

    fn generate_goals(&self, world: &WorldState) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        for (color, assignment) in &self.color_assignments {
            let player = &world.players[assignment.player_index];

            let goal = match assignment.phase {
                KeyDoorPhase::FetchKey => {
                    // Verify we still know where the key is and can reach it
                    if world.knows_key_location(*color) {
                        if let Some(key_pos) = world.closest_key(player, *color) {
                            let can_reach = world.find_path(player.position, key_pos).is_some();
                            if can_reach {
                                Some(Goal::GetKey(*color))
                            } else {
                                debug!(
                                    "Player {} cannot reach {:?} key anymore",
                                    assignment.player_index + 1,
                                    color
                                );
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                KeyDoorPhase::OpenDoor => {
                    // Verify player still has the key and door is reachable
                    if world.has_key(player, *color) {
                        // Check if any door of this color has a reachable neighbor
                        if let Some(door_positions) = world.doors.get_positions(*color) {
                            let mut can_reach = false;
                            for &door_pos in door_positions {
                                for &neighbor in &door_pos.neighbors() {
                                    if neighbor == player.position
                                        || (matches!(
                                            world.map.get(&neighbor),
                                            Some(crate::swoq_interface::Tile::Empty)
                                        ) && world
                                            .find_path(player.position, neighbor)
                                            .is_some())
                                    {
                                        can_reach = true;
                                        break;
                                    }
                                }
                                if can_reach {
                                    break;
                                }
                            }

                            if can_reach {
                                Some(Goal::OpenDoor(*color))
                            } else {
                                debug!(
                                    "Player {} cannot reach {:?} door anymore",
                                    assignment.player_index + 1,
                                    color
                                );
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        debug!(
                            "Player {} no longer has {:?} key",
                            assignment.player_index + 1,
                            color
                        );
                        None
                    }
                }
            };

            if let Some(g) = goal {
                goals[assignment.player_index] = Some(g);
            }
        }

        goals
    }
}

impl SelectGoal for KeyAndDoorStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        // Clean up invalid assignments
        self.clean_assignments(world);

        // Update phases based on player inventory
        self.update_phases(world);

        // Try to assign new colors to available players
        self.assign_new_colors(world);

        // Generate goals based on current assignments
        let goals = self.generate_goals(world);

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
