use std::collections::HashMap;
use tracing::debug;

use crate::infra::Color;
use crate::planners::heuristic::goals::Goal;
use crate::planners::heuristic::pathfinding::find_path_with_custom_walkability;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::planners::heuristic::strategies::planner::{SelectGoal, StrategyType};

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

    #[tracing::instrument(level = "debug", skip(self, state))]
    fn clean_assignments(&mut self, state: &PlannerState) {
        // Remove assignments for colors where the door has been opened or is no longer relevant
        self.color_assignments.retain(|color, assignment| {
            // Keep if door still exists and hasn't been opened
            if !state.world.doors.has_color(*color) {
                debug!("Removing assignment for {:?}: door no longer exists", color);
                return false;
            }

            // Check if door has been opened (by key or by pressure plate with boulder)
            if state.world.has_door_been_opened(*color) {
                debug!("Removing assignment for {:?}: door has been opened", color);
                return false;
            }

            // Check if assigned player still exists and is active
            if assignment.player_index >= state.world.players.len()
                || !state.world.players[assignment.player_index].is_active
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

    #[tracing::instrument(level = "debug", skip(self, state))]
    fn update_phases(&mut self, state: &PlannerState) {
        // Update phases based on current player inventory
        for (color, assignment) in self.color_assignments.iter_mut() {
            let player = &state.world.players[assignment.player_index];

            // If player has the key, advance to OpenDoor phase
            if assignment.phase == KeyDoorPhase::FetchKey && state.world.has_key(player, *color) {
                debug!(
                    "Player {} has acquired {:?} key, advancing to OpenDoor phase",
                    assignment.player_index + 1,
                    color
                );
                assignment.phase = KeyDoorPhase::OpenDoor;
            }
        }
    }

    #[tracing::instrument(level = "debug", skip(self, state))]
    fn assign_new_colors(&mut self, state: &PlannerState) {
        debug!("assign_new_colors: Starting assignment");

        // Get colors that need assignment
        let unassigned_colors: Vec<Color> = state
            .world
            .doors
            .colors()
            .filter(|color| {
                !self.color_assignments.contains_key(color)
                    && !state.world.has_door_been_opened(**color)
            })
            .copied()
            .collect();

        debug!("assign_new_colors: Unassigned colors: {:?}", unassigned_colors);
        debug!("assign_new_colors: Current assignments: {:?}", self.color_assignments);

        // In 2-player mode, check which door colors have reachable pressure plates
        let mut doors_with_plates = std::collections::HashSet::new();
        if state.world.is_two_player_mode() {
            debug!("assign_new_colors: In 2-player mode, checking pressure plates");
            for &color in &[Color::Red, Color::Green, Color::Blue] {
                if let Some(plates) = state.world.pressure_plates.get_positions(color) {
                    debug!("assign_new_colors: {:?} has {} pressure plates", color, plates.len());
                    let can_reach_plate = state.world.players.iter().any(|player| {
                        plates.iter().any(|&plate_pos| {
                            state.world.find_path(player.position, plate_pos).is_some()
                        })
                    });
                    if can_reach_plate {
                        doors_with_plates.insert(color);
                        debug!("assign_new_colors: {:?} door has reachable pressure plate", color);
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

        debug!("assign_new_colors: Already assigned players: {:?}", assigned_players);

        for color in unassigned_colors {
            debug!("assign_new_colors: Processing {:?}", color);

            // First check if any available player already has the key (picked up accidentally)
            let mut player_with_key = None;
            for (player_index, player) in state.world.players.iter().enumerate() {
                if player.is_active
                    && !assigned_players.contains(&player_index)
                    && state.world.has_key(player, color)
                {
                    debug!(
                        "assign_new_colors: Player {} already has {:?} key (picked up accidentally)",
                        player_index + 1,
                        color
                    );
                    player_with_key = Some(player_index);
                    break;
                }
            }

            if let Some(player_index) = player_with_key {
                debug!(
                    "assign_new_colors: ✓ Assigning {:?} to player {} (OpenDoor phase - already has key)",
                    color,
                    player_index + 1
                );
                self.color_assignments.insert(
                    color,
                    ColorAssignment {
                        player_index,
                        phase: KeyDoorPhase::OpenDoor,
                    },
                );
                continue;
            }

            // Check if we know where the key is
            if !state.world.knows_key_location(color) {
                debug!("assign_new_colors: Don't know location of {:?} key", color);
                continue;
            }

            // Find the best available player for this color
            let mut best_player: Option<(usize, i32)> = None;

            for (player_index, player) in state.world.players.iter().enumerate() {
                if !player.is_active {
                    debug!("assign_new_colors: Player {} is not active", player_index + 1);
                    continue;
                }
                if assigned_players.contains(&player_index) {
                    debug!("assign_new_colors: Player {} already assigned", player_index + 1);
                    continue;
                }

                if let Some(key_pos) = state.world.closest_key(player, color) {
                    debug!(
                        "assign_new_colors: Player {} at {:?}, closest {:?} key at {:?}",
                        player_index + 1,
                        player.position,
                        color,
                        key_pos
                    );

                    // In 2-player mode with reachable pressure plate, treat matching doors as walkable
                    let use_custom_walkability =
                        state.world.is_two_player_mode() && doors_with_plates.contains(&color);

                    debug!(
                        "assign_new_colors: Player {}, use_custom_walkability={}",
                        player_index + 1,
                        use_custom_walkability
                    );

                    let can_reach = if use_custom_walkability {
                        find_path_with_custom_walkability(
                            &state.world,
                            player.position,
                            key_pos,
                            |pos, goal, _tick| {
                                let is_matching_door = matches!(
                                    (state.world.map.get(pos), color),
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
                                    state.world.is_walkable(pos, Some(goal))
                                }
                            },
                        )
                        .is_some()
                    } else {
                        state.world.find_path(player.position, key_pos).is_some()
                    };

                    debug!(
                        "assign_new_colors: Player {} can_reach {:?} key: {}",
                        player_index + 1,
                        color,
                        can_reach
                    );

                    if can_reach {
                        let distance = player.position.distance(&key_pos);
                        debug!(
                            "assign_new_colors: Player {} distance to {:?} key: {}",
                            player_index + 1,
                            color,
                            distance
                        );

                        best_player = match best_player {
                            None => Some((player_index, distance)),
                            Some((_, best_dist)) if distance < best_dist => {
                                Some((player_index, distance))
                            }
                            _ => best_player,
                        };
                    }
                } else {
                    debug!(
                        "assign_new_colors: No {:?} key found for player {}",
                        color,
                        player_index + 1
                    );
                }
            }

            // Assign the color to the best player
            if let Some((player_index, distance)) = best_player {
                debug!(
                    "assign_new_colors: ✓ Assigning {:?} to player {} (FetchKey phase), distance={}",
                    color,
                    player_index + 1,
                    distance
                );
                self.color_assignments.insert(
                    color,
                    ColorAssignment {
                        player_index,
                        phase: KeyDoorPhase::FetchKey,
                    },
                );
            } else {
                debug!("assign_new_colors: ✗ No suitable player found for {:?}", color);
            }
        }

        debug!("assign_new_colors: Final assignments: {:?}", self.color_assignments);
    }

    #[tracing::instrument(level = "debug", skip(self, state))]
    fn generate_goals(&self, state: &PlannerState) -> Vec<Option<Goal>> {
        let mut goals = vec![None; state.world.players.len()];

        for (color, assignment) in &self.color_assignments {
            let player = &state.world.players[assignment.player_index];

            let goal = match assignment.phase {
                KeyDoorPhase::FetchKey => {
                    // Verify we still know where the key is and can reach it
                    if state.world.knows_key_location(*color) {
                        if let Some(key_pos) = state.world.closest_key(player, *color) {
                            let can_reach =
                                state.world.find_path(player.position, key_pos).is_some();
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
                    if state.world.has_key(player, *color) {
                        // Check if any door of this color has a reachable neighbor
                        if let Some(door_positions) = state.world.doors.get_positions(*color) {
                            let mut can_reach = false;
                            for &door_pos in door_positions {
                                for &neighbor in &door_pos.neighbors() {
                                    if neighbor == player.position
                                        || (matches!(
                                            state.world.map.get(&neighbor),
                                            Some(crate::swoq_interface::Tile::Empty)
                                        ) && state
                                            .world
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

    #[tracing::instrument(
        level = "debug",
        skip(self, state, current_goals),
        fields(strategy = "KeyAndDoorStrategy")
    )]
    fn try_select_coop(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        // Clean up invalid assignments
        self.clean_assignments(state);

        // Update phases based on player inventory
        self.update_phases(state);

        // Try to assign new colors to available players
        self.assign_new_colors(state);

        // Generate goals based on current assignments
        let goals = self.generate_goals(state);

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
