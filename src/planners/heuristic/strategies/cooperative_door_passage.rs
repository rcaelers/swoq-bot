use std::collections::HashMap;
use tracing::debug;

use crate::planners::heuristic::goals::Goal;
use crate::planners::heuristic::strategies::planner::StrategyPlanner;
use crate::planners::heuristic::strategies::planner::{SelectGoal, StrategyType};
use crate::infra::{Color, Position};
use crate::planners::heuristic::planner_state::PlannerState;

/// Helper struct to hold pathfinding results for a single player
struct PlayerReachability {
    can_reach_plate: bool,
    can_reach_door: bool,
    distance_to_plate: usize,
    path_to_door: Option<Vec<Position>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CooperativeDoorPassageState {
    Setup,
    ExecuteNavigating, // Passing player moving to target, waiting player on plate
    ExecuteReleasing,  // Passing player at target, waiting player leaving plate
    ExecuteWaiting, // Passing player at target, waiting player left plate, wait for door to close
}

pub struct CooperativeDoorPassageStrategy {
    state: CooperativeDoorPassageState,
    // Track when each color door was last opened using a plate (tick number)
    last_plate_door_usage: HashMap<Color, i32>,
}

impl CooperativeDoorPassageStrategy {
    pub fn new() -> Self {
        Self {
            state: CooperativeDoorPassageState::Setup,
            last_plate_door_usage: HashMap::new(),
        }
    }

    /// Check if there's an active cooperative door passage in progress
    fn has_active_door_cooperation(&self, state: &PlannerState) -> bool {
        state.player_states.iter().enumerate().any(|(idx, ps)| {
            if let Some(Goal::WaitOnTile(color, plate_pos)) = &ps.previous_goal {
                let other_idx = 1 - idx;
                if let Some(Goal::PassThroughDoor(other_color, door_pos, target_pos)) =
                    &state.player_states[other_idx].previous_goal
                    && color == other_color
                {
                    debug!(
                        "Active cooperation detected - P{} waiting on plate at {:?}, P{} passing through door at {:?} to {:?}",
                        idx + 1,
                        plate_pos,
                        other_idx + 1,
                        door_pos,
                        target_pos
                    );
                    return true;
                }
            }
            false
        })
    }

    /// Check if state transition is needed and update state accordingly
    fn check_state_transition(&mut self, state: &PlannerState) {
        // Only works in 2-player mode
        if state.world.players.len() != 2 {
            return;
        }

        match self.state {
            CooperativeDoorPassageState::ExecuteNavigating => {
                // Check if passing player reached target -> transition to ExecuteReleasing
                if let Some((_, _, passing_idx, _, target_pos)) = self.find_cooperation_pair(state)
                    && state.world.players[passing_idx].position == target_pos
                {
                    debug!(
                        "CooperativeDoorPassageStrategy: Transition ExecuteNavigating -> ExecuteReleasing"
                    );
                    self.state = CooperativeDoorPassageState::ExecuteReleasing;
                }
            }
            CooperativeDoorPassageState::ExecuteReleasing => {
                // Check if waiting player left plate -> transition to ExecuteWaiting
                if let Some((waiter_idx, plate_pos, _, _, _)) = self.find_cooperation_pair(state)
                    && state.world.players[waiter_idx].position != plate_pos
                {
                    debug!(
                        "CooperativeDoorPassageStrategy: Transition ExecuteReleasing -> ExecuteWaiting"
                    );
                    self.state = CooperativeDoorPassageState::ExecuteWaiting;
                }
            }
            CooperativeDoorPassageState::ExecuteWaiting => {
                // Check if cooperation ended -> transition to Setup
                if self.find_cooperation_pair(state).is_none() {
                    debug!(
                        "CooperativeDoorPassageStrategy: Transition ExecuteWaiting -> Setup (cooperation ended)"
                    );
                    self.state = CooperativeDoorPassageState::Setup;
                }
            }
            CooperativeDoorPassageState::Setup => {
                // No transition check needed in Setup state
            }
        }
    }

    /// Find the active cooperation pair (waiter_idx, plate_pos, passing_idx, door_pos, target_pos)
    fn find_cooperation_pair(
        &self,
        state: &PlannerState,
    ) -> Option<(usize, Position, usize, Position, Position)> {
        for player_index in 0..2 {
            if let Some(Goal::WaitOnTile(color, plate_pos)) =
                state.player_states[player_index].previous_goal.as_ref()
            {
                let other_player_index = 1 - player_index;
                if let Some(Goal::PassThroughDoor(c, door_pos, target_pos)) =
                    state.player_states[other_player_index].previous_goal.as_ref()
                    && c == color
                {
                    return Some((
                        player_index,
                        *plate_pos,
                        other_player_index,
                        *door_pos,
                        *target_pos,
                    ));
                }
            }
        }
        None
    }

    #[tracing::instrument(level = "debug", skip(self, state))]
    fn execute_navigating(&mut self, state: &PlannerState) -> Vec<Option<Goal>> {
        debug!("CooperativeDoorPassageStrategy: ExecuteNavigating state");

        if let Some((waiter_idx, plate_pos, passing_idx, door_pos, target_pos)) =
            self.find_cooperation_pair(state)
        {
            let passing_player = &state.world.players[passing_idx];

            // State 1: Passing player navigating to target, waiting player on plate
            if passing_player.position != target_pos {
                debug!(
                    "CooperativeDoorPassageStrategy: STATE 1 - P{} navigating to target {:?} (current: {:?}), P{} waiting on plate at {:?}",
                    passing_idx + 1,
                    target_pos,
                    passing_player.position,
                    waiter_idx + 1,
                    plate_pos
                );
                let mut goals = vec![None; 2];

                // Extract colors from previous goals, with fallback to match each other
                let waiter_color = state.player_states[waiter_idx]
                    .previous_goal
                    .as_ref()
                    .and_then(|g| {
                        if let Goal::WaitOnTile(c, _) = g {
                            Some(c)
                        } else {
                            None
                        }
                    });
                let passer_color =
                    state.player_states[passing_idx]
                        .previous_goal
                        .as_ref()
                        .and_then(|g| {
                            if let Goal::PassThroughDoor(c, _, _) = g {
                                Some(c)
                            } else {
                                None
                            }
                        });
                let color = waiter_color.or(passer_color).copied().unwrap_or(Color::Red);

                // Only assign WaitOnTile if waiter doesn't have an emergency goal (e.g., attack/flee)
                if !state.player_states[waiter_idx].current_goal.as_ref().is_some_and(|g| matches!(g, Goal::KillEnemy(_) | Goal::AvoidEnemy(_))) {
                    goals[waiter_idx] = Some(Goal::WaitOnTile(color, plate_pos));
                }
                goals[passing_idx] = Some(Goal::PassThroughDoor(color, door_pos, target_pos));
                return goals;
            }
        }

        debug!("CooperativeDoorPassageStrategy: No active cooperation found in ExecuteNavigating");
        self.state = CooperativeDoorPassageState::Setup;
        vec![None; 2]
    }

    #[tracing::instrument(level = "debug", skip(self, state))]
    fn execute_releasing(&mut self, state: &PlannerState) -> Vec<Option<Goal>> {
        debug!("CooperativeDoorPassageStrategy: ExecuteReleasing state");

        if let Some((waiter_idx, plate_pos, passing_idx, door_pos, target_pos)) =
            self.find_cooperation_pair(state)
        {
            let waiting_player = &state.world.players[waiter_idx];

            // State 2: Passing player at target, waiting player still on plate - release waiting player
            if waiting_player.position == plate_pos {
                debug!(
                    "CooperativeDoorPassageStrategy: STATE 2 - P{} at target {:?}, P{} on plate at {:?} - RELEASE P{} to leave",
                    passing_idx + 1,
                    target_pos,
                    waiter_idx + 1,
                    plate_pos,
                    waiter_idx + 1
                );
                let mut goals = vec![None; 2];
                goals[waiter_idx] = None; // Release waiting player to move off plate
                goals[passing_idx] = Some(Goal::PassThroughDoor(
                    state.player_states[passing_idx]
                        .previous_goal
                        .as_ref()
                        .and_then(|g| {
                            if let Goal::PassThroughDoor(c, _, _) = g {
                                Some(c)
                            } else {
                                None
                            }
                        })
                        .copied()
                        .unwrap_or(Color::Red),
                    door_pos,
                    target_pos,
                )); // Passing player stays at target
                return goals;
            }
        }

        debug!("CooperativeDoorPassageStrategy: No active cooperation found in ExecuteReleasing");
        self.state = CooperativeDoorPassageState::Setup;
        vec![None; 2]
    }

    #[tracing::instrument(level = "debug", skip(self, state))]
    fn execute_waiting(&mut self, state: &PlannerState) -> Vec<Option<Goal>> {
        debug!("CooperativeDoorPassageStrategy: ExecuteWaiting state");

        // Check both possible cooperation directions
        for player_index in 0..2 {
            // Case 1: Check if this player is the passer who reached target
            if let Some(Goal::PassThroughDoor(color, door_pos, target_pos)) =
                state.player_states[player_index].previous_goal.as_ref()
            {
                let passing_player = &state.world.players[player_index];
                let other_player_index = 1 - player_index;
                let other_player = &state.world.players[other_player_index];

                // If passing player is at target, keep them frozen
                if passing_player.position == *target_pos {
                    debug!(
                        "CooperativeDoorPassageStrategy: STATE 3/4 - P{} at target {:?}, P{} free to move",
                        player_index + 1,
                        target_pos,
                        other_player_index + 1
                    );

                    // Check if other player is still on a matching plate (door still open)
                    let other_on_plate =
                        if let Some(plates) = state.world.pressure_plates.get_positions(*color) {
                            plates.contains(&other_player.position)
                        } else {
                            false
                        };

                    if other_on_plate {
                        debug!(
                            "CooperativeDoorPassageStrategy: P{} still on plate, P{} stays frozen at target",
                            other_player_index + 1,
                            player_index + 1
                        );
                    } else {
                        debug!(
                            "CooperativeDoorPassageStrategy: P{} left plate, P{} stays frozen until door closes",
                            other_player_index + 1,
                            player_index + 1
                        );
                    }

                    let mut goals = vec![None; 2];
                    goals[player_index] =
                        Some(Goal::PassThroughDoor(*color, *door_pos, *target_pos)); // Passing player stays
                    goals[other_player_index] = None; // Other player is free
                    return goals;
                }
            }
        }

        debug!("CooperativeDoorPassageStrategy: No active cooperation found in ExecuteWaiting");
        self.state = CooperativeDoorPassageState::Setup;
        vec![None; 2]
    }

    /// Calculate pathfinding for a single player to the plate and door
    fn player_can_reach_plate_and_door(
        &self,
        state: &PlannerState,
        player_pos: Position,
        plate_pos: Position,
        door_pos: Position,
    ) -> PlayerReachability {
        debug!(
            "player_can_reach_plate_and_door: player at {:?}, plate at {:?}, door at {:?}",
            player_pos, plate_pos, door_pos
        );

        let path_to_plate = state.world.find_path(player_pos, plate_pos);
        let can_reach_plate = path_to_plate.is_some();
        let distance_to_plate = path_to_plate
            .as_ref()
            .map(|p| p.len())
            .unwrap_or(i32::MAX as usize);

        debug!(
            "player_can_reach_plate_and_door: can_reach_plate={}, distance={}",
            can_reach_plate, distance_to_plate
        );

        // Check each neighbor of the door
        let door_neighbors = door_pos.neighbors();
        debug!(
            "player_can_reach_plate_and_door: door neighbors: {:?}",
            door_neighbors
        );

        let path_to_door = door_pos.neighbors().iter().find_map(|&neighbor| {
            let tile = state.world.map.get(&neighbor);
            // Consider a neighbor valid if it's empty OR if it's the player's current position
            let is_valid = matches!(tile, Some(crate::swoq_interface::Tile::Empty)) 
                || neighbor == player_pos;
            debug!(
                "player_can_reach_plate_and_door: checking neighbor {:?}, tile={:?}, is_valid={}, is_player_pos={}",
                neighbor, tile, is_valid, neighbor == player_pos
            );

            if is_valid {
                let path = state.world.find_path(player_pos, neighbor);
                debug!(
                    "player_can_reach_plate_and_door: path from {:?} to {:?}: {}",
                    player_pos,
                    neighbor,
                    if let Some(p) = path.as_ref() {
                        format!("found (len={})", p.len())
                    } else {
                        "None".to_string()
                    }
                );
                path
            } else {
                None
            }
        });
        let can_reach_door = path_to_door.is_some();

        debug!(
            "player_can_reach_plate_and_door: can_reach_door={}, path_len={}",
            can_reach_door,
            path_to_door.as_ref().map(|p| p.len()).unwrap_or(0)
        );

        PlayerReachability {
            can_reach_plate,
            can_reach_door,
            distance_to_plate,
            path_to_door,
        }
    }

    /// Calculate the target position one step beyond the door
    fn calculate_target_beyond_door(
        &self,
        door_pos: Position,
        path_to_door: &[Position],
    ) -> Option<Position> {
        if path_to_door.is_empty() {
            return None;
        }
        let last = path_to_door[path_to_door.len() - 1];
        let dx = door_pos.x - last.x;
        let dy = door_pos.y - last.y;
        Some(Position {
            x: door_pos.x + dx,
            y: door_pos.y + dy,
        })
    }

    /// Check if a specific player can already reach the target position
    fn is_target_already_reachable(&self, state: &PlannerState, player_index: usize, target_pos: Position) -> bool {
        state.world
            .find_path(state.world.players[player_index].position, target_pos)
            .is_some()
    }

    #[tracing::instrument(level = "debug", skip(self, state, current_goals))]
    fn setup_phase(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("CoopPressurePlateDoorStrategy: Starting setup phase evaluation");

        if !state.world.is_two_player_mode() {
            debug!(
                "CoopPressurePlateDoorStrategy: Not 2-player mode ({}), skipping",
                state.world.players.len()
            );
            return vec![None; state.world.players.len()];
        }

        if !StrategyPlanner::all_players_have_no_goals(current_goals) {
            debug!(
                "CoopPressurePlateDoorStrategy: Some players already have goals, skipping new assignment"
            );
            return vec![None; state.world.players.len()];
        }

        if state.world.any_player_has_frontier() {
            debug!(
                "CoopPressurePlateDoorStrategy: Some players still have unexplored frontier, continuing exploration first"
            );
            return vec![None; state.world.players.len()];
        }

        if state.world.has_boulders_not_on_plates() {
            debug!(
                "CoopPressurePlateDoorStrategy: Boulders not on plates exist, preferring boulder solution"
            );
            return vec![None; state.world.players.len()];
        }

        // Sort colors by least recently used (prefer colors not used yet or used longest ago)
        let mut colors_by_usage: Vec<Color> = vec![Color::Red, Color::Green, Color::Blue];
        colors_by_usage.sort_by_key(|&color| {
            self.last_plate_door_usage
                .get(&color)
                .copied()
                .unwrap_or(i32::MIN)
        });
        debug!(
            "CoopPressurePlateDoorStrategy: Checking colors in priority order: {:?}",
            colors_by_usage
        );

        // Find a pressure plate and door of the same color
        for color in colors_by_usage {
            debug!("CoopPressurePlateDoorStrategy: Checking {:?} color", color);

            // Skip if any player has a key for this color
            if state.world.players.iter().any(|p| state.world.has_key(p, color)) {
                debug!("CoopPressurePlateDoorStrategy: Player has {:?} key, skipping", color);
                continue;
            }

            let door_positions = match state.world.doors.get_positions(color) {
                Some(pos) => pos,
                None => {
                    debug!("CoopPressurePlateDoorStrategy: No {:?} doors found", color);
                    continue;
                }
            };
            if door_positions.is_empty() {
                debug!("CoopPressurePlateDoorStrategy: {:?} door positions empty", color);
                continue;
            }

            debug!(
                "CoopPressurePlateDoorStrategy: Found {} {:?} doors",
                door_positions.len(),
                color
            );

            let plates = match state.world.pressure_plates.get_positions(color) {
                Some(p) => p,
                None => {
                    debug!("CoopPressurePlateDoorStrategy: No {:?} pressure plates found", color);
                    continue;
                }
            };

            debug!("CoopPressurePlateDoorStrategy: Found {} {:?} plates", plates.len(), color);

            let last_usage_tick = self
                .last_plate_door_usage
                .get(&color)
                .copied()
                .unwrap_or(i32::MIN);
            debug!(
                "CoopPressurePlateDoorStrategy: {:?} door last used at tick {} (current tick: {})",
                color,
                if last_usage_tick == i32::MIN {
                    "never".to_string()
                } else {
                    last_usage_tick.to_string()
                },
                state.world.tick
            );

            // Find a pressure plate that's reachable by at least one player
            for &plate_pos in plates {
                debug!("CoopPressurePlateDoorStrategy: Checking plate at {:?}", plate_pos);

                // Check if there's a door that's NOT adjacent to this plate
                // (we want to find doors that need someone to wait on the plate)
                for &door_pos in door_positions {
                    let p0_reach = self.player_can_reach_plate_and_door(
                        state,
                        state.world.players[0].position,
                        plate_pos,
                        door_pos,
                    );
                    let p1_reach = self.player_can_reach_plate_and_door(
                        state,
                        state.world.players[1].position,
                        plate_pos,
                        door_pos,
                    );

                    debug!(
                        "CoopPressurePlateDoorStrategy: P1 path dist to plate: {}, P2 path dist to plate: {}",
                        if p0_reach.distance_to_plate == i32::MAX as usize {
                            "unreachable".to_string()
                        } else {
                            p0_reach.distance_to_plate.to_string()
                        },
                        if p1_reach.distance_to_plate == i32::MAX as usize {
                            "unreachable".to_string()
                        } else {
                            p1_reach.distance_to_plate.to_string()
                        }
                    );

                    debug!(
                        "CoopPressurePlateDoorStrategy: P1 can reach plate: {}, door: {}",
                        p0_reach.can_reach_plate, p0_reach.can_reach_door
                    );
                    debug!(
                        "CoopPressurePlateDoorStrategy: P2 can reach plate: {}, door: {}",
                        p1_reach.can_reach_plate, p1_reach.can_reach_door
                    );

                    // Assign roles: closer player to plate waits, other goes through door
                    let (waiter_idx, passer_idx, passer_reach) =
                        if p0_reach.distance_to_plate <= p1_reach.distance_to_plate {
                            (0, 1, &p1_reach)
                        } else {
                            (1, 0, &p0_reach)
                        };

                    if let Some(ref path_to_door) = passer_reach.path_to_door
                        && let Some(target_pos) =
                            self.calculate_target_beyond_door(door_pos, path_to_door)
                    {
                        let last = path_to_door[path_to_door.len() - 1];
                        debug!(
                            "CoopPressurePlateDoorStrategy: Door at {:?}, neighbor at {:?}, target {:?}",
                            door_pos, last, target_pos
                        );

                        if self.is_target_already_reachable(state, passer_idx, target_pos) {
                            debug!(
                                "CoopPressurePlateDoorStrategy: Target {:?} is already reachable by P{}, no cooperation needed",
                                target_pos, passer_idx + 1
                            );
                            continue;
                        }

                        // Record this door color as being used with a plate at this tick
                        self.last_plate_door_usage.insert(color, state.world.tick);

                        // Transition to ExecuteNavigating state
                        self.state = CooperativeDoorPassageState::ExecuteNavigating;

                        debug!(
                            "CoopPressurePlateDoorStrategy: ✓ SELECTED - P{} waits on {:?} plate at {:?}, P{} goes through door at {:?} to target {:?} (last used: tick {})",
                            waiter_idx + 1,
                            color,
                            plate_pos,
                            passer_idx + 1,
                            door_pos,
                            target_pos,
                            state.world.tick
                        );

                        let mut goals = vec![None; 2];
                        goals[waiter_idx] = Some(Goal::WaitOnTile(color, plate_pos));
                        goals[passer_idx] = Some(Goal::PassThroughDoor(color, last, target_pos));
                        return goals;
                    }

                    debug!(
                        "CoopPressurePlateDoorStrategy: Could not calculate valid target for P{} through door at {:?}",
                        passer_idx + 1,
                        door_pos
                    );
                    continue;
                }
            }
        }

        debug!("CoopPressurePlateDoorStrategy: No suitable plate/door combination found");
        vec![None; state.world.players.len()]
    }
}

impl SelectGoal for CooperativeDoorPassageStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn is_emergency(&self) -> bool {
        // Emergency strategy in Execute states to maintain coordination
        !matches!(self.state, CooperativeDoorPassageState::Setup)
    }

    fn prioritize(&self, state: &PlannerState) -> bool {
        // In any Execute state, prioritize if there's active cooperation
        match self.state {
            CooperativeDoorPassageState::ExecuteNavigating
            | CooperativeDoorPassageState::ExecuteReleasing
            | CooperativeDoorPassageState::ExecuteWaiting => {
                self.has_active_door_cooperation(state)
            }
            CooperativeDoorPassageState::Setup => false,
        }
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, state, current_goals),
        fields(strategy = "CooperativeDoorPassageStrategy")
    )]
    fn try_select_coop(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("CooperativeDoorPassageStrategy");
        match self.state {
            CooperativeDoorPassageState::Setup => self.setup_phase(state, current_goals),
            CooperativeDoorPassageState::ExecuteNavigating
            | CooperativeDoorPassageState::ExecuteReleasing
            | CooperativeDoorPassageState::ExecuteWaiting => {
                // Only works in 2-player mode
                if state.world.players.len() != 2 {
                    return vec![None; 2];
                }

                // Check if state transition is needed
                self.check_state_transition(state);

                debug!(
                    "CooperativeDoorPassageStrategy: Checking cooperation - P1 prev goal: {:?}, P2 prev goal: {:?}",
                    state.player_states[0].previous_goal, state.player_states[1].previous_goal
                );
                debug!(
                    "CooperativeDoorPassageStrategy: P1 pos: {:?}, P2 pos: {:?}",
                    state.world.players[0].position, state.world.players[1].position
                );

                // Dispatch to appropriate state handler
                match self.state {
                    CooperativeDoorPassageState::ExecuteNavigating => {
                        self.execute_navigating(state)
                    }
                    CooperativeDoorPassageState::ExecuteReleasing => self.execute_releasing(state),
                    CooperativeDoorPassageState::ExecuteWaiting => self.execute_waiting(state),
                    CooperativeDoorPassageState::Setup => {
                        // Should not happen, but return empty goals
                        vec![None; 2]
                    }
                }
            }
        }
    }
}
